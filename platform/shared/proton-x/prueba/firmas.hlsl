// firmas.hlsl -- las root signatures de firmas.exe (06-10): la misma, en
// 1.1 (con banderas: la que mete `dxc` en un sombreador) y en 1.0, y un CS
// que la trae DENTRO ([RootSignature]).
//
// La raiz: 0, cuatro constantes (b0); 1, una tabla con dos SRV (t0, t1) y
// un UAV (u1); 2, un UAV en la raiz (u0).
#define RS11 "RootFlags(0), RootConstants(num32BitConstants=4, b0), DescriptorTable(SRV(t0, numDescriptors=2, flags=DATA_STATIC), UAV(u1, flags=DESCRIPTORS_VOLATILE)), UAV(u0, flags=DATA_VOLATILE)"
#define RS10 "RootFlags(0), RootConstants(num32BitConstants=4, b0), DescriptorTable(SRV(t0, numDescriptors=2), UAV(u1)), UAV(u0)"

cbuffer K : register(b0) {
    uint4 k;
};
StructuredBuffer<uint> a : register(t0);
StructuredBuffer<uint> b : register(t1);
RWStructuredBuffer<uint> tabla : register(u1);
RWByteAddressBuffer raiz : register(u0);

// Cada hilo: a * k.x + b * k.y + k.w (k.w, el caso), a la raiz y a su
// sitio de la tabla (4 por caso), mas 1000.
[RootSignature(RS11)]
[numthreads(4, 1, 1)]
void CSFirma(uint i : SV_DispatchThreadID) {
    uint v = a[i] * k.x + b[i] * k.y + k.w;
    raiz.Store(4 * i, v);
    tabla[4 * k.w + i] = v + 1000;
}
