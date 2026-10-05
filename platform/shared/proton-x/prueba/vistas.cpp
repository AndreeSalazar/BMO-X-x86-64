// vistas.cpp -- el juez de las VISTAS de D3D12 que la casa no veia (N5.3b y
// N5.3c de docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10), con los CS de
// vistas.hlsl (aqui al lado):
//
//   A  todo en la RAIZ (SetComputeRootShaderResourceView y
//      ...UnorderedAccessView): un SRV estructurado de 32 bytes por elemento
//      (su paso lo dice el sombreador, no la vista), un UAV estructurado y
//      uno crudo.
//   B  un RWTexture2D de 16 x 16 (R8G8B8A8_UNORM) en una tabla, con sus
//      medidas de GetDimensions: cada texel, su (x, y) / 15.
//   C  la misma textura LEIDA como UAV, a un RWBuffer con tipo
//      R8G8B8A8_UNORM, con los canales al reves (bgra).
//   D  ClearUnorderedAccessViewUint de un bufer crudo (0xDEADBEEF) y
//      ClearUnorderedAccessViewFloat de una textura de 8 x 8 (un UNORM
//      satura: 2.0 es 255).
//
// Todo se lee y se compara con la cuenta, BIT A BIT. Sale con el numero de
// fallos. En Windows dice lo mismo (es de consola: lo que falle, se lee).
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

__asm__(".section .rdata,\"dr\"\n"
        ".p2align 4\n"
        ".globl cs_raiz\n cs_raiz:\n .incbin \"vistas_raiz.dxil\"\n .globl cs_raiz_fin\n cs_raiz_fin:\n"
        ".p2align 4\n"
        ".globl cs_imagen\n cs_imagen:\n .incbin \"vistas_imagen.dxil\"\n .globl cs_imagen_fin\n cs_imagen_fin:\n"
        ".p2align 4\n"
        ".globl cs_lee\n cs_lee:\n .incbin \"vistas_lee.dxil\"\n .globl cs_lee_fin\n cs_lee_fin:\n"
        ".text\n");
extern "C" const unsigned char cs_raiz[], cs_raiz_fin[], cs_imagen[], cs_imagen_fin[], cs_lee[], cs_lee_fin[];

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

static void barrera_uav(ID3D12GraphicsCommandList *l, ID3D12Resource *b) {
    D3D12_RESOURCE_BARRIER x = {};
    x.Type = D3D12_RESOURCE_BARRIER_TYPE_UAV;
    x.UAV.pResource = b;
    l->ResourceBarrier(1, &x);
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

// Copiar una textura (`ancho` x `alto`, RGBA8) a `leida` desde `desde`, con
// filas de 256 bytes (lo que pide D3D12).
static void leer_textura(ID3D12GraphicsCommandList *l, ID3D12Resource *t, ID3D12Resource *leida, UINT64 desde, UINT ancho, UINT alto) {
    D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
    a.pResource = leida;
    a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
    a.PlacedFootprint.Offset = desde;
    a.PlacedFootprint.Footprint = {DXGI_FORMAT_R8G8B8A8_UNORM, ancho, alto, 1, 256};
    de.pResource = t;
    de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
    l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
}

static UINT rgba(UINT r, UINT g, UINT b, UINT a) {
    return r | g << 8 | b << 16 | a << 24;
}

// Comparar `n` palabras con lo que tienen que ser.
static void juzgar(const char *que, const UINT *leido, const UINT *quiero, UINT n) {
    UINT malos = 0, primero = 0;
    for (UINT i = 0; i < n; i++)
        if (leido[i] != quiero[i] && malos++ == 0)
            primero = i;
    char m[240];
    if (malos == 0)
        snprintf(m, sizeof m, "%s: las %u palabras bit a bit", que, n);
    else
        snprintf(m, sizeof m, "%s: %u distintas; la %u es %08x y tenia que ser %08x", que, malos, primero, leido[primero], quiero[primero]);
    decir(malos == 0, m);
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

    // La root signature de los tres: [0] SRV t0, [1] UAV u0, [2] UAV u1 en la
    // raiz; [3] una tabla con u0 del espacio 1; [4] otra con u0 y u1 del 2.
    D3D12_DESCRIPTOR_RANGE r1 = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 0, 1, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND};
    D3D12_DESCRIPTOR_RANGE r2 = {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 2, 0, 2, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND};
    D3D12_ROOT_PARAMETER ps[5] = {};
    ps[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_SRV;
    ps[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    ps[2].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    ps[2].Descriptor.ShaderRegister = 1;
    ps[3].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[3].DescriptorTable = {1, &r1};
    ps[4].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    ps[4].DescriptorTable = {1, &r2};
    D3D12_ROOT_SIGNATURE_DESC rd = {5, ps, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *firma = nullptr, *error = nullptr;
    ID3D12RootSignature *rs = nullptr;
    if (hecho(D3D12SerializeRootSignature(&rd, D3D_ROOT_SIGNATURE_VERSION_1, &firma, &error), "D3D12SerializeRootSignature"))
        hecho(d->CreateRootSignature(0, firma->GetBufferPointer(), firma->GetBufferSize(), IID_PPV_ARGS(&rs)), "CreateRootSignature");
    if (fallos)
        return fallos;
    ID3D12PipelineState *pa = pso(d, rs, cs_raiz, cs_raiz_fin), *pb = pso(d, rs, cs_imagen, cs_imagen_fin), *pc = pso(d, rs, cs_lee, cs_lee_fin);
    if (fallos)
        return fallos;
    decir(true, "CreateComputePipelineState: los tres CS con vistas en la raiz y en tablas");

    // Los recursos.
    const D3D12_RESOURCE_FLAGS UAV = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    float datos[8][8];
    for (UINT i = 0; i < 8; i++) {
        float p[8] = {(float)(2 * i), 0, 0, (float)i, 0, 10, 0, 0};
        memcpy(datos[i], p, sizeof p);
    }
    ID3D12Resource *pares = bufer(d, D3D12_HEAP_TYPE_UPLOAD, sizeof datos, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_GENERIC_READ);
    void *m = nullptr;
    D3D12_RANGE nada = {0, 0};
    if (pares && hecho(pares->Map(0, &nada, &m), "Map"))
        memcpy(m, datos, sizeof datos);
    ID3D12Resource *sumas = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 64, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    ID3D12Resource *crudo = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 32, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    ID3D12Resource *destino = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 1024, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    ID3D12Resource *relleno = bufer(d, D3D12_HEAP_TYPE_DEFAULT, 64, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    ID3D12Resource *imagen = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, 16, 16, DXGI_FORMAT_R8G8B8A8_UNORM, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    ID3D12Resource *lisa = recurso(d, D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_DIMENSION_TEXTURE2D, 8, 8, DXGI_FORMAT_R8G8B8A8_UNORM, UAV, D3D12_RESOURCE_STATE_UNORDERED_ACCESS);
    // La lectura: sumas 0, crudo 64, destino 128, relleno 1152, imagen 1280
    // (16 filas de 256), lisa 5376 (8 filas).
    ID3D12Resource *leida = bufer(d, D3D12_HEAP_TYPE_READBACK, 5376 + 8 * 256, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST);
    if (fallos)
        return fallos;

    // Los descriptores: el monton que ve el sombreador ([0] imagen para B,
    // [1] imagen para C, [2] destino con tipo, [3] relleno crudo, [4] lisa) y
    // el de la CPU para las limpiezas ([0] relleno, [1] lisa).
    D3D12_DESCRIPTOR_HEAP_DESC hd = {D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, 5, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, 0};
    ID3D12DescriptorHeap *visible = nullptr, *de_cpu = nullptr;
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&visible)), "CreateDescriptorHeap visible");
    hd.NumDescriptors = 2;
    hd.Flags = D3D12_DESCRIPTOR_HEAP_FLAG_NONE;
    hecho(d->CreateDescriptorHeap(&hd, IID_PPV_ARGS(&de_cpu)), "CreateDescriptorHeap de CPU");
    if (fallos)
        return fallos;
    UINT paso = d->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
    D3D12_CPU_DESCRIPTOR_HANDLE cv, cc;
    D3D12_GPU_DESCRIPTOR_HANDLE gv;
    visible->GetCPUDescriptorHandleForHeapStart(&cv);
    visible->GetGPUDescriptorHandleForHeapStart(&gv);
    de_cpu->GetCPUDescriptorHandleForHeapStart(&cc);
    auto en = [&](D3D12_CPU_DESCRIPTOR_HANDLE h, UINT i) { return D3D12_CPU_DESCRIPTOR_HANDLE{h.ptr + (SIZE_T)i * paso}; };
    auto eng = [&](UINT i) { return D3D12_GPU_DESCRIPTOR_HANDLE{gv.ptr + (UINT64)i * paso}; };
    D3D12_UNORDERED_ACCESS_VIEW_DESC tex = {};
    tex.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    tex.ViewDimension = D3D12_UAV_DIMENSION_TEXTURE2D;
    d->CreateUnorderedAccessView(imagen, nullptr, &tex, en(cv, 0));
    d->CreateUnorderedAccessView(imagen, nullptr, &tex, en(cv, 1));
    D3D12_UNORDERED_ACCESS_VIEW_DESC tipado = {};
    tipado.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    tipado.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    tipado.Buffer.NumElements = 256;
    d->CreateUnorderedAccessView(destino, nullptr, &tipado, en(cv, 2));
    D3D12_UNORDERED_ACCESS_VIEW_DESC raw = {};
    raw.Format = DXGI_FORMAT_R32_TYPELESS;
    raw.ViewDimension = D3D12_UAV_DIMENSION_BUFFER;
    raw.Buffer.NumElements = 16;
    raw.Buffer.Flags = D3D12_BUFFER_UAV_FLAG_RAW;
    d->CreateUnorderedAccessView(relleno, nullptr, &raw, en(cv, 3));
    d->CreateUnorderedAccessView(relleno, nullptr, &raw, en(cc, 0));
    d->CreateUnorderedAccessView(lisa, nullptr, &tex, en(cv, 4));
    d->CreateUnorderedAccessView(lisa, nullptr, &tex, en(cc, 1));

    ID3D12GraphicsCommandList *l = nullptr;
    if (!hecho(d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, pa, IID_PPV_ARGS(&l)), "CreateCommandList"))
        return fallos;
    l->SetDescriptorHeaps(1, &visible);
    l->SetComputeRootSignature(rs);
    // D: las limpiezas.
    const UINT muerto[4] = {0xDEADBEEF, 0, 0, 0};
    const float azul[4] = {0.25f, 0.5f, 1.0f, 2.0f};
    l->ClearUnorderedAccessViewUint(eng(3), en(cc, 0), relleno, muerto, 0, nullptr);
    l->ClearUnorderedAccessViewFloat(eng(4), en(cc, 1), lisa, azul, 0, nullptr);
    // A.
    l->SetComputeRootShaderResourceView(0, pares->GetGPUVirtualAddress());
    l->SetComputeRootUnorderedAccessView(1, sumas->GetGPUVirtualAddress());
    l->SetComputeRootUnorderedAccessView(2, crudo->GetGPUVirtualAddress());
    l->Dispatch(1, 1, 1);
    // B y C.
    l->SetPipelineState(pb);
    l->SetComputeRootDescriptorTable(3, eng(0));
    l->Dispatch(4, 4, 1);
    barrera_uav(l, imagen);
    l->SetPipelineState(pc);
    l->SetComputeRootDescriptorTable(4, eng(1));
    l->Dispatch(4, 4, 1);
    // Leer.
    ID3D12Resource *todos[6] = {sumas, crudo, destino, relleno, imagen, lisa};
    for (ID3D12Resource *x : todos)
        barrera(l, x, D3D12_RESOURCE_STATE_UNORDERED_ACCESS, D3D12_RESOURCE_STATE_COPY_SOURCE);
    l->CopyBufferRegion(leida, 0, sumas, 0, 64);
    l->CopyBufferRegion(leida, 64, crudo, 0, 32);
    l->CopyBufferRegion(leida, 128, destino, 0, 1024);
    l->CopyBufferRegion(leida, 1152, relleno, 0, 64);
    leer_textura(l, imagen, leida, 1280, 16, 16);
    leer_textura(l, lisa, leida, 5376, 8, 8);
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

    // Lo que tiene que salir.
    UINT q[256];
    for (UINT i = 0; i < 8; i++) {
        q[2 * i] = 2 * i + 10;
        q[2 * i + 1] = 2 * i;
    }
    juzgar("A, en la RAIZ: el SRV estructurado de 32 bytes y el UAV estructurado", (const UINT *)b, q, 16);
    for (UINT i = 0; i < 8; i++)
        q[i] = 3 * i + 1;
    juzgar("A, en la RAIZ: el UAV crudo", (const UINT *)(b + 64), q, 8);
    UINT img[256];
    for (UINT y = 0; y < 16; y++)
        for (UINT x = 0; x < 16; x++)
            img[y * 16 + x] = ((const UINT *)(b + 1280 + y * 256))[x];
    for (UINT y = 0; y < 16; y++)
        for (UINT x = 0; x < 16; x++)
            q[y * 16 + x] = rgba(x * 17, y * 17, 128, 255);
    juzgar("B, RWTexture2D con GetDimensions: cada texel, su (x, y) / 15", img, q, 256);
    for (UINT y = 0; y < 16; y++)
        for (UINT x = 0; x < 16; x++)
            q[y * 16 + x] = rgba(128, y * 17, x * 17, 255);
    juzgar("C, la textura leida como UAV, a un RWBuffer con tipo R8G8B8A8_UNORM", (const UINT *)(b + 128), q, 256);
    for (UINT i = 0; i < 16; i++)
        q[i] = 0xDEADBEEF;
    juzgar("D, ClearUnorderedAccessViewUint de un bufer crudo", (const UINT *)(b + 1152), q, 16);
    UINT lisa_leida[64];
    for (UINT y = 0; y < 8; y++)
        for (UINT x = 0; x < 8; x++)
            lisa_leida[y * 8 + x] = ((const UINT *)(b + 5376 + y * 256))[x];
    for (UINT i = 0; i < 64; i++)
        q[i] = rgba(64, 128, 255, 255);
    juzgar("D, ClearUnorderedAccessViewFloat de una textura UNORM (satura)", lisa_leida, q, 64);
    printf("vistas.exe: las vistas de D3D12 son las de Windows\n");
    return fallos;
}
