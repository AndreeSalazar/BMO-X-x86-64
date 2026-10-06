// leefirma.cpp -- el juez de los DESERIALIZADORES de root signatures y de
// la 1.1 serializada con los bytes de Windows (A3 y A4 del contador de
// docs/plan/PLAN_LAS_TRES_GRANDES.md 7.1, 06-10). La firma es la de
// firmas.hlsl, la que `dxc` serializo en la 1.1 (firmas_11.rts0: con
// banderas DATA_STATIC, DESCRIPTORS_VOLATILE y DATA_VOLATILE) y en la 1.0
// (firmas_10.rts0); los dos van dentro.
//
//   A  D3D12SerializeVersionedRootSignature de su DESC1: los bytes de dxc.
//   B  D3D12SerializeRootSignature de su DESC (1.0): los bytes de dxc.
//   C  el deserializador con versiones, sobre la 1.1: sin convertir, la 1.1
//      con sus banderas (8, 1 y 2), sus registros y sus cuentas.
//   D  y pedida en la 1.0: los mismos rangos, sin banderas.
//   E  sobre la 1.0, pedida en la 1.1: las banderas que D3D12 le da a la
//      1.0 (los rangos, DESCRIPTORS_VOLATILE | DATA_VOLATILE: 3; el UAV de
//      la raiz, DATA_VOLATILE: 2).
//   F  D3D12CreateRootSignatureDeserializer, sobre la 1.0: su DESC.
//   G  lo que vuelve de C, serializado otra vez: los bytes de dxc.
//   H  una con un sampler estatico, ida y vuelta, y pedida en la 1.2 (el
//      sampler con sus Flags, a 0).
//   I  unos bytes que no son una firma: E_INVALIDARG en las dos.
//
// Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

#define BLOB(n, f) ".p2align 4\n.globl " #n "\n " #n ":\n .incbin \"" f "\"\n .globl " #n "_fin\n " #n "_fin:\n"
__asm__(".section .rdata,\"dr\"\n" BLOB(rs11, "firmas_11.rts0") BLOB(rs10, "firmas_10.rts0") ".text\n");
extern "C" const unsigned char rs11[], rs11_fin[], rs10[], rs10_fin[];

static int fallos = 0;

static void decir(bool bien, const char *que) {
    printf("%s%s\n", bien ? "  bien  " : "  MAL   ", que);
    if (!bien)
        fallos++;
}

// Un blob igual a [p, fin): cuantos bytes y el primero distinto, dichos.
static void mismos(const char *que, ID3DBlob *b, const unsigned char *p, const unsigned char *fin) {
    char m[240];
    size_t n = (size_t)(fin - p);
    if (!b) {
        snprintf(m, sizeof m, "%s: no salio el blob", que);
        decir(false, m);
        return;
    }
    const unsigned char *q = (const unsigned char *)b->GetBufferPointer();
    size_t k = b->GetBufferSize(), i = 0;
    while (i < k && i < n && q[i] == p[i])
        i++;
    if (k == n && i == n)
        snprintf(m, sizeof m, "%s: los %zu bytes de dxc, huella incluida", que, n);
    else
        snprintf(m, sizeof m, "%s: %zu bytes (dxc %zu); el primero distinto, el %zu (%02x y dxc %02x)", que, k, n, i, i < k ? q[i] : 0, i < n ? p[i] : 0);
    decir(k == n && i == n, m);
}

// Lo de la firma de firmas.hlsl, con las banderas de la 1.1 (o 0 en la 1.0).
static bool la_de_firmas(const D3D12_ROOT_SIGNATURE_DESC1 &d, UINT b_srv, UINT b_uav, UINT b_raiz, char *m, size_t n) {
    if (d.NumParameters != 3 || d.NumStaticSamplers != 0 || d.Flags != 0) {
        snprintf(m, n, "%u parametros, %u samplers, banderas %x", d.NumParameters, d.NumStaticSamplers, d.Flags);
        return false;
    }
    const D3D12_ROOT_PARAMETER1 *p = d.pParameters;
    bool c = p[0].ParameterType == D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS && p[0].Constants.ShaderRegister == 0 && p[0].Constants.Num32BitValues == 4;
    bool t = p[1].ParameterType == D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE && p[1].DescriptorTable.NumDescriptorRanges == 2;
    const D3D12_DESCRIPTOR_RANGE1 *r = t ? p[1].DescriptorTable.pDescriptorRanges : nullptr;
    bool rs = r && r[0].RangeType == D3D12_DESCRIPTOR_RANGE_TYPE_SRV && r[0].NumDescriptors == 2 && r[0].BaseShaderRegister == 0 && r[0].Flags == b_srv && r[0].OffsetInDescriptorsFromTableStart == D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND &&
              r[1].RangeType == D3D12_DESCRIPTOR_RANGE_TYPE_UAV && r[1].NumDescriptors == 1 && r[1].BaseShaderRegister == 1 && r[1].Flags == b_uav && r[1].OffsetInDescriptorsFromTableStart == D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND;
    bool u = p[2].ParameterType == D3D12_ROOT_PARAMETER_TYPE_UAV && p[2].Descriptor.ShaderRegister == 0 && p[2].Descriptor.Flags == b_raiz;
    snprintf(m, n, "constantes %d, tabla %d (banderas %x y %x), UAV de la raiz %d (banderas %x)", c, rs, r ? r[0].Flags : 0, r ? r[1].Flags : 0, u, p[2].Descriptor.Flags);
    return c && t && rs && u;
}

int main() {
    char m[240], que[240];
    // A: la DESC1 de la firma, como la escribiria el juego.
    D3D12_DESCRIPTOR_RANGE1 r1[2] = {
        {D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 2, 0, 0, D3D12_DESCRIPTOR_RANGE_FLAG_DATA_STATIC, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND},
        {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 1, 0, D3D12_DESCRIPTOR_RANGE_FLAG_DESCRIPTORS_VOLATILE, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND},
    };
    D3D12_ROOT_PARAMETER1 p1[3] = {};
    p1[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    p1[0].Constants = {0, 0, 4};
    p1[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    p1[1].DescriptorTable = {2, r1};
    p1[2].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    p1[2].Descriptor = {0, 0, D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE};
    D3D12_VERSIONED_ROOT_SIGNATURE_DESC v = {};
    v.Version = D3D_ROOT_SIGNATURE_VERSION_1_1;
    v.Desc_1_1 = {3, p1, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    ID3DBlob *b11 = nullptr, *b10 = nullptr, *err = nullptr;
    HRESULT h = D3D12SerializeVersionedRootSignature(&v, &b11, &err);
    mismos("A, D3D12SerializeVersionedRootSignature de la 1.1", SUCCEEDED(h) ? b11 : nullptr, rs11, rs11_fin);
    // B: la misma en la 1.0.
    D3D12_DESCRIPTOR_RANGE r0[2] = {{D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 2, 0, 0, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND}, {D3D12_DESCRIPTOR_RANGE_TYPE_UAV, 1, 1, 0, D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND}};
    D3D12_ROOT_PARAMETER p0[3] = {};
    p0[0].ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
    p0[0].Constants = {0, 0, 4};
    p0[1].ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
    p0[1].DescriptorTable = {2, r0};
    p0[2].ParameterType = D3D12_ROOT_PARAMETER_TYPE_UAV;
    D3D12_ROOT_SIGNATURE_DESC d0 = {3, p0, 0, nullptr, D3D12_ROOT_SIGNATURE_FLAG_NONE};
    h = D3D12SerializeRootSignature(&d0, D3D_ROOT_SIGNATURE_VERSION_1, &b10, &err);
    mismos("B, D3D12SerializeRootSignature de la 1.0", SUCCEEDED(h) ? b10 : nullptr, rs10, rs10_fin);

    // C, D y G: el deserializador con versiones, sobre la 1.1 de dxc.
    ID3D12VersionedRootSignatureDeserializer *dv = nullptr;
    h = D3D12CreateVersionedRootSignatureDeserializer(rs11, (SIZE_T)(rs11_fin - rs11), IID_PPV_ARGS(&dv));
    if (FAILED(h) || !dv) {
        snprintf(m, sizeof m, "C, D3D12CreateVersionedRootSignatureDeserializer: HRESULT 0x%08lx", (unsigned long)h);
        decir(false, m);
    } else {
        const D3D12_VERSIONED_ROOT_SIGNATURE_DESC *u = dv->GetUnconvertedRootSignatureDesc();
        bool ok = u && u->Version == D3D_ROOT_SIGNATURE_VERSION_1_1 && la_de_firmas(u->Desc_1_1, 8, 1, 2, que, sizeof que);
        snprintf(m, sizeof m, "C, sin convertir: la version %u; %s", u ? u->Version : 0, u ? que : "nada");
        decir(ok, m);
        const D3D12_VERSIONED_ROOT_SIGNATURE_DESC *e = nullptr;
        h = dv->GetRootSignatureDescAtVersion(D3D_ROOT_SIGNATURE_VERSION_1_0, &e);
        ok = SUCCEEDED(h) && e && e->Version == D3D_ROOT_SIGNATURE_VERSION_1_0;
        if (ok) {
            const D3D12_ROOT_SIGNATURE_DESC &d = e->Desc_1_0;
            const D3D12_DESCRIPTOR_RANGE *r = d.NumParameters == 3 ? d.pParameters[1].DescriptorTable.pDescriptorRanges : nullptr;
            ok = r && d.pParameters[1].DescriptorTable.NumDescriptorRanges == 2 && r[0].NumDescriptors == 2 && r[1].RangeType == D3D12_DESCRIPTOR_RANGE_TYPE_UAV && r[1].BaseShaderRegister == 1 &&
                 r[1].OffsetInDescriptorsFromTableStart == D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND && d.pParameters[2].Descriptor.ShaderRegister == 0;
        }
        snprintf(m, sizeof m, "D, pedida en la 1.0: HRESULT 0x%08lx, la version %u, sus rangos de 20 bytes", (unsigned long)h, e ? e->Version : 0);
        decir(ok, m);
        ID3DBlob *otra = nullptr;
        h = u ? D3D12SerializeVersionedRootSignature(u, &otra, &err) : E_FAIL;
        mismos("G, lo que devolvio C, serializado otra vez", SUCCEEDED(h) ? otra : nullptr, rs11, rs11_fin);
    }

    // E: sobre la 1.0, pedida en la 1.1.
    ID3D12VersionedRootSignatureDeserializer *dv0 = nullptr;
    h = D3D12CreateVersionedRootSignatureDeserializer(rs10, (SIZE_T)(rs10_fin - rs10), IID_PPV_ARGS(&dv0));
    const D3D12_VERSIONED_ROOT_SIGNATURE_DESC *e1 = nullptr;
    if (SUCCEEDED(h) && dv0)
        h = dv0->GetRootSignatureDescAtVersion(D3D_ROOT_SIGNATURE_VERSION_1_1, &e1);
    bool ok = SUCCEEDED(h) && e1 && e1->Version == D3D_ROOT_SIGNATURE_VERSION_1_1 && la_de_firmas(e1->Desc_1_1, 3, 3, 2, que, sizeof que);
    snprintf(m, sizeof m, "E, la 1.0 pedida en la 1.1: HRESULT 0x%08lx; %s", (unsigned long)h, e1 ? que : "nada");
    decir(ok, m);

    // F: el deserializador sin versiones.
    ID3D12RootSignatureDeserializer *d10 = nullptr;
    h = D3D12CreateRootSignatureDeserializer(rs10, (SIZE_T)(rs10_fin - rs10), IID_PPV_ARGS(&d10));
    const D3D12_ROOT_SIGNATURE_DESC *f = (SUCCEEDED(h) && d10) ? d10->GetRootSignatureDesc() : nullptr;
    ok = f && f->NumParameters == 3 && f->pParameters[0].Constants.Num32BitValues == 4 && f->pParameters[1].DescriptorTable.NumDescriptorRanges == 2 && f->pParameters[1].DescriptorTable.pDescriptorRanges[0].NumDescriptors == 2;
    snprintf(m, sizeof m, "F, D3D12CreateRootSignatureDeserializer: HRESULT 0x%08lx, %u parametros", (unsigned long)h, f ? f->NumParameters : 0);
    decir(ok, m);

    // H: una con un sampler estatico.
    D3D12_STATIC_SAMPLER_DESC s = {};
    s.Filter = D3D12_FILTER_MIN_MAG_MIP_LINEAR;
    s.AddressU = s.AddressV = s.AddressW = D3D12_TEXTURE_ADDRESS_MODE_CLAMP;
    s.MaxAnisotropy = 1;
    s.ComparisonFunc = D3D12_COMPARISON_FUNC_NEVER;
    s.MaxLOD = 7.5f;
    s.ShaderRegister = 3;
    s.RegisterSpace = 2;
    s.ShaderVisibility = D3D12_SHADER_VISIBILITY_PIXEL;
    D3D12_ROOT_SIGNATURE_DESC ds = {0, nullptr, 1, &s, D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT};
    ID3DBlob *bs = nullptr;
    h = D3D12SerializeRootSignature(&ds, D3D_ROOT_SIGNATURE_VERSION_1, &bs, &err);
    ID3D12VersionedRootSignatureDeserializer *dvs = nullptr;
    if (SUCCEEDED(h))
        h = D3D12CreateVersionedRootSignatureDeserializer(bs->GetBufferPointer(), bs->GetBufferSize(), IID_PPV_ARGS(&dvs));
    const D3D12_VERSIONED_ROOT_SIGNATURE_DESC *e0 = nullptr, *e2 = nullptr;
    HRESULT h2 = E_FAIL;
    if (SUCCEEDED(h) && dvs) {
        e0 = dvs->GetUnconvertedRootSignatureDesc();
        h2 = dvs->GetRootSignatureDescAtVersion(D3D_ROOT_SIGNATURE_VERSION_1_2, &e2);
    }
    ok = e0 && e0->Desc_1_0.NumStaticSamplers == 1 && memcmp(e0->Desc_1_0.pStaticSamplers, &s, sizeof s) == 0 && e0->Desc_1_0.Flags == D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT;
    snprintf(m, sizeof m, "H, un sampler estatico ida y vuelta: HRESULT 0x%08lx, %u samplers, el mismo %d", (unsigned long)h, e0 ? e0->Desc_1_0.NumStaticSamplers : 0, ok);
    decir(ok, m);
    bool ok2 = SUCCEEDED(h2) && e2 && e2->Version == D3D_ROOT_SIGNATURE_VERSION_1_2 && e2->Desc_1_2.NumStaticSamplers == 1 && memcmp(e2->Desc_1_2.pStaticSamplers, &s, sizeof s) == 0 && e2->Desc_1_2.pStaticSamplers[0].Flags == 0;
    snprintf(m, sizeof m, "H, y pedida en la 1.2: HRESULT 0x%08lx, el sampler con sus Flags a 0: %d", (unsigned long)h2, ok2);
    decir(ok2, m);

    // I: unos bytes que no son una firma.
    const unsigned char basura[64] = {'D', 'X', 'B', 'C'};
    ID3D12RootSignatureDeserializer *x = nullptr;
    ID3D12VersionedRootSignatureDeserializer *y = nullptr;
    HRESULT hx = D3D12CreateRootSignatureDeserializer(basura, sizeof basura, IID_PPV_ARGS(&x));
    HRESULT hy = D3D12CreateVersionedRootSignatureDeserializer(basura, sizeof basura, IID_PPV_ARGS(&y));
    snprintf(m, sizeof m, "I, unos bytes que no son una firma: 0x%08lx y 0x%08lx (E_INVALIDARG es 0x80070057)", (unsigned long)hx, (unsigned long)hy);
    decir(hx == E_INVALIDARG && hy == E_INVALIDARG, m);

    printf("leefirma.exe: los deserializadores y la 1.1 serializada son los de Windows\n");
    return fallos;
}
