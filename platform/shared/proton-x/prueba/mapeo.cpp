// mapeo.cpp -- el juez de Map sobre una TEXTURA (06-10, A7 del contador de
// DX12). Un motor que sube sus texturas sin bufer intermedio (en una GPU
// integrada, o con un monton CUSTOM de paginas de la CPU) hace
// Map(sub, NULL, NULL), WriteToSubresource, Unmap; hasta el 06-10 la casa
// decia E_INVALIDARG en el Map ("Map sobre algo que no es un bufer").
//
//   A  CreateCommittedResource de una textura 2D RGBA8 de 8 x 8 con 2 mips
//      en un monton CUSTOM (WRITE_BACK, L0), layout UNKNOWN.
//   B  Map(0, NULL, NULL) y Map(1, NULL, NULL): S_OK.
//   C  WriteToSubresource de las dos mips entera y de una caja de la 0.
//   D  ReadFromSubresource: lo escrito, texel a texel.
//   E  La GPU lo ve: CopyTextureRegion a un bufer de lectura, las dos mips.
//   F  Map de una textura de un monton DEFAULT: E_INVALIDARG (no es de la
//      CPU).
//   G  GetHeapProperties de la de A: CUSTOM, WRITE_BACK, L0 (la casa daba
//      la pagina y la piscina a 0).
//   nota  Map(0, NULL, &p) de la textura de layout UNKNOWN: lo que diga
//      Windows (su HRESULT).
//
// Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

static int fallos = 0;

static void decir(bool bien, const char *que) {
    printf("%s%s\n", bien ? "  bien  " : "  MAL   ", que);
    if (!bien)
        fallos++;
}

static D3D12_RESOURCE_DESC textura(UINT lado, UINT16 mips) {
    D3D12_RESOURCE_DESC r = {};
    r.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    r.Width = lado;
    r.Height = lado;
    r.DepthOrArraySize = 1;
    r.MipLevels = mips;
    r.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    r.SampleDesc.Count = 1;
    r.Layout = D3D12_TEXTURE_LAYOUT_UNKNOWN;
    return r;
}

// El texel (x, y) de la mip m, como se escribe: 0xAABBGGRR.
static UINT texel(UINT m, UINT x, UINT y) { return 0xFF000000u | (m << 16) | (y << 8) | x; }

int main() {
    ID3D12Device *d = nullptr;
    HRESULT h = D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d));
    if (FAILED(h)) {
        printf("  MAL   D3D12CreateDevice: HRESULT 0x%08lx\n", (unsigned long)h);
        return 1;
    }
    char msg[240];

    // A: la textura en un monton CUSTOM de la CPU.
    D3D12_HEAP_PROPERTIES custom = {};
    custom.Type = D3D12_HEAP_TYPE_CUSTOM;
    custom.CPUPageProperty = D3D12_CPU_PAGE_PROPERTY_WRITE_BACK;
    custom.MemoryPoolPreference = D3D12_MEMORY_POOL_L0;
    D3D12_RESOURCE_DESC dt = textura(8, 2);
    ID3D12Resource *t = nullptr;
    h = d->CreateCommittedResource(&custom, D3D12_HEAP_FLAG_NONE, &dt, D3D12_RESOURCE_STATE_COMMON, nullptr, IID_PPV_ARGS(&t));
    snprintf(msg, sizeof msg, "A, una textura 2D de 8 x 8 con 2 mips en un monton CUSTOM (WRITE_BACK, L0): HRESULT 0x%08lx", (unsigned long)h);
    decir(SUCCEEDED(h), msg);
    if (FAILED(h))
        return fallos;

    // B: Map de sus dos subrecursos, sin puntero.
    HRESULT m0 = t->Map(0, nullptr, nullptr), m1 = t->Map(1, nullptr, nullptr);
    snprintf(msg, sizeof msg, "B, Map(0, NULL, NULL) y Map(1, NULL, NULL): 0x%08lx y 0x%08lx", (unsigned long)m0, (unsigned long)m1);
    decir(m0 == S_OK && m1 == S_OK, msg);

    // C: las dos mips enteras, y luego una caja de 3 x 2 en (4, 5) de la 0
    // (con el texel de la mip 2, que no existe: se ve que es la caja).
    UINT mip0[64], mip1[16], caja[6];
    for (UINT y = 0; y < 8; y++)
        for (UINT x = 0; x < 8; x++)
            mip0[y * 8 + x] = texel(0, x, y);
    for (UINT y = 0; y < 4; y++)
        for (UINT x = 0; x < 4; x++)
            mip1[y * 4 + x] = texel(1, x, y);
    for (UINT y = 0; y < 2; y++)
        for (UINT x = 0; x < 3; x++)
            caja[y * 3 + x] = texel(2, x + 4, y + 5);
    D3D12_BOX b = {4, 5, 0, 7, 7, 1};
    HRESULT w0 = t->WriteToSubresource(0, nullptr, mip0, 32, 256), w1 = t->WriteToSubresource(1, nullptr, mip1, 16, 64), w2 = t->WriteToSubresource(0, &b, caja, 12, 24);
    snprintf(msg, sizeof msg, "C, WriteToSubresource de la mip 0, de la 1 y de una caja de la 0: 0x%08lx, 0x%08lx, 0x%08lx", (unsigned long)w0, (unsigned long)w1, (unsigned long)w2);
    decir(w0 == S_OK && w1 == S_OK && w2 == S_OK, msg);
    for (UINT y = 5; y < 7; y++)
        for (UINT x = 4; x < 7; x++)
            mip0[y * 8 + x] = texel(2, x, y);

    // D: de vuelta, por ReadFromSubresource.
    UINT leido0[64] = {}, leido1[16] = {};
    HRESULT r0 = t->ReadFromSubresource(leido0, 32, 256, 0, nullptr), r1 = t->ReadFromSubresource(leido1, 16, 64, 1, nullptr);
    t->Unmap(0, nullptr);
    t->Unmap(1, nullptr);
    UINT malos = 0;
    for (UINT i = 0; i < 64; i++)
        malos += leido0[i] != mip0[i];
    for (UINT i = 0; i < 16; i++)
        malos += leido1[i] != mip1[i];
    snprintf(msg, sizeof msg, "D, ReadFromSubresource (0x%08lx, 0x%08lx): %u de 80 texeles distintos; el (5, 6) de la 0 es %08x", (unsigned long)r0, (unsigned long)r1, malos, leido0[6 * 8 + 5]);
    decir(r0 == S_OK && r1 == S_OK && malos == 0, msg);

    // E: la GPU copia las dos mips a un bufer de lectura.
    D3D12_COMMAND_QUEUE_DESC qd = {};
    qd.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    ID3D12CommandQueue *cola = nullptr;
    ID3D12CommandAllocator *al = nullptr;
    ID3D12GraphicsCommandList *l = nullptr;
    ID3D12Fence *valla = nullptr;
    d->CreateCommandQueue(&qd, IID_PPV_ARGS(&cola));
    d->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&al));
    d->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, al, nullptr, IID_PPV_ARGS(&l));
    d->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&valla));
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT hu[2];
    UINT64 total = 0;
    d->GetCopyableFootprints(&dt, 0, 2, 0, hu, nullptr, nullptr, &total);
    D3D12_HEAP_PROPERTIES lectura = {};
    lectura.Type = D3D12_HEAP_TYPE_READBACK;
    D3D12_RESOURCE_DESC db = {};
    db.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    db.Width = total;
    db.Height = 1;
    db.DepthOrArraySize = 1;
    db.MipLevels = 1;
    db.SampleDesc.Count = 1;
    db.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    ID3D12Resource *leida = nullptr;
    d->CreateCommittedResource(&lectura, D3D12_HEAP_FLAG_NONE, &db, D3D12_RESOURCE_STATE_COPY_DEST, nullptr, IID_PPV_ARGS(&leida));
    if (!cola || !al || !l || !valla || !leida) {
        decir(false, "E, la cola, la lista o el bufer de lectura no se crearon");
        return fallos;
    }
    for (UINT s = 0; s < 2; s++) {
        D3D12_TEXTURE_COPY_LOCATION a = {}, de = {};
        a.pResource = leida;
        a.Type = D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT;
        a.PlacedFootprint = hu[s];
        de.pResource = t;
        de.Type = D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX;
        de.SubresourceIndex = s;
        l->CopyTextureRegion(&a, 0, 0, 0, &de, nullptr);
    }
    l->Close();
    ID3D12CommandList *ls[1] = {l};
    cola->ExecuteCommandLists(1, ls);
    cola->Signal(valla, 1);
    HANDLE ev = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (valla->GetCompletedValue() < 1) {
        valla->SetEventOnCompletion(1, ev);
        WaitForSingleObject(ev, 10000);
    }
    unsigned char *p = nullptr;
    malos = 0;
    if (SUCCEEDED(leida->Map(0, nullptr, (void **)&p))) {
        for (UINT y = 0; y < 8; y++)
            for (UINT x = 0; x < 8; x++)
                malos += *(const UINT *)(p + hu[0].Offset + (UINT64)y * hu[0].Footprint.RowPitch + 4 * x) != mip0[y * 8 + x];
        for (UINT y = 0; y < 4; y++)
            for (UINT x = 0; x < 4; x++)
                malos += *(const UINT *)(p + hu[1].Offset + (UINT64)y * hu[1].Footprint.RowPitch + 4 * x) != mip1[y * 4 + x];
    } else {
        malos = 80;
    }
    snprintf(msg, sizeof msg, "E, la GPU ve lo escrito: CopyTextureRegion de las dos mips, %u de 80 texeles distintos", malos);
    decir(malos == 0, msg);

    // F: una textura de un monton DEFAULT no se mapea.
    D3D12_HEAP_PROPERTIES def = {};
    def.Type = D3D12_HEAP_TYPE_DEFAULT;
    D3D12_RESOURCE_DESC d1 = textura(8, 1);
    ID3D12Resource *t2 = nullptr;
    HRESULT c2 = d->CreateCommittedResource(&def, D3D12_HEAP_FLAG_NONE, &d1, D3D12_RESOURCE_STATE_COMMON, nullptr, IID_PPV_ARGS(&t2));
    HRESULT m2 = t2 ? t2->Map(0, nullptr, nullptr) : E_FAIL;
    snprintf(msg, sizeof msg, "F, Map de una textura de un monton DEFAULT: 0x%08lx (pide E_INVALIDARG)", (unsigned long)m2);
    decir(SUCCEEDED(c2) && m2 == E_INVALIDARG, msg);

    // G: de que monton es la de A.
    D3D12_HEAP_PROPERTIES hp = {};
    D3D12_HEAP_FLAGS hb = D3D12_HEAP_FLAG_NONE;
    HRESULT g = t->GetHeapProperties(&hp, &hb);
    snprintf(msg, sizeof msg, "G, GetHeapProperties de la textura de A (0x%08lx): tipo %u, pagina %u, piscina %u (pide 4, 3, 1)", (unsigned long)g, (unsigned)hp.Type, (unsigned)hp.CPUPageProperty, (unsigned)hp.MemoryPoolPreference);
    decir(g == S_OK && hp.Type == D3D12_HEAP_TYPE_CUSTOM && hp.CPUPageProperty == D3D12_CPU_PAGE_PROPERTY_WRITE_BACK && hp.MemoryPoolPreference == D3D12_MEMORY_POOL_L0, msg);

    // La nota: Map con puntero de una textura de layout UNKNOWN.
    void *q = nullptr;
    HRESULT m3 = t->Map(0, nullptr, &q);
    printf("  nota  Map(0, NULL, &p) de una textura de layout UNKNOWN en un CUSTOM: HRESULT 0x%08lx, %s\n", (unsigned long)m3, q ? "con puntero" : "sin puntero");
    if (SUCCEEDED(m3))
        t->Unmap(0, nullptr);

    printf("mapeo.exe: Map sobre una textura es el de Windows\n");
    return fallos;
}
