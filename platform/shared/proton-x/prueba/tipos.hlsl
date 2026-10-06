// tipos.hlsl -- los sombreadores de tipos.exe (D2.7, 06-10): una textura
// vista con OTRO formato del mismo tamanio. Los recursos son TYPELESS (de 4
// x 4): t8 R8G8B8A8, t10 R10G10B10A2, t16 R16G16B16A16. Cuentas exactas.

RWTexture2D<unorm float4> t8_unorm : register(u0); // t8 como R8G8B8A8_UNORM
RWTexture2D<uint> t8_palabra : register(u1);       // t8 como R32_UINT
RWTexture2D<uint> t10_palabra : register(u2);      // t10 como R32_UINT
RWTexture2D<float4> t16_half : register(u3);       // t16 como R16G16B16A16_FLOAT
RWByteAddressBuffer sal : register(u4);

Texture2D<float4> t10_unorm : register(t0); // t10 como R10G10B10A2_UNORM
Texture2D<uint4> t16_uint : register(t1);   // t16 como R16G16B16A16_UINT
Texture2D<int4> t8_sint : register(t2);     // t8 como R8G8B8A8_SINT

// A: cada hilo escribe su texel de las tres, por la vista de su tipo.
[numthreads(4, 4, 1)]
void CSEscribe(uint2 p : SV_DispatchThreadID) {
    t8_unorm[p] = float4(0.2 * p.x, 0.2 * p.y, 1.0, 0.0);
    uint r = 300 * p.x + p.y;
    t10_palabra[p] = r | (1023 - r) << 10 | 512u << 20 | ((p.x + p.y) & 3) << 30;
    t16_half[p] = float4(p.x, -(float)p.y, 0.5, 65504.0);
}

// B: por la vista R32_UINT de t8, un InterlockedAdd de 1 (sube el R) y lo
// que habia, a `sal`.
[numthreads(4, 4, 1)]
void CSSuma(uint2 p : SV_DispatchThreadID) {
    uint antes;
    InterlockedAdd(t8_palabra[p], 1, antes);
    sal.Store(4 * (p.y * 4 + p.x), antes);
}

// C: las tres leidas por un SRV de otro tipo (Load), a `sal`.
[numthreads(4, 4, 1)]
void CSLee(uint2 p : SV_DispatchThreadID) {
    uint i = p.y * 4 + p.x;
    float4 a = t10_unorm.Load(int3(p, 0));
    sal.Store4(64 + 16 * i, uint4(round(a * float4(1023, 1023, 1023, 3))));
    sal.Store4(320 + 16 * i, t16_uint.Load(int3(p, 0)));
    sal.Store4(576 + 16 * i, asuint(t8_sint.Load(int3(p, 0))));
    if (i == 0) {
        uint w, h;
        t8_sint.GetDimensions(w, h);
        sal.Store2(832, uint2(w, h));
    }
}

// D: un triangulo que cubre los 4 x 4, en DOS render targets TYPELESS de
// RGBA8 vistos como R8G8B8A8_UINT y R8G8B8A8_SNORM.
float4 VSLleno(uint id : SV_VertexID) : SV_Position {
    float2 c = float2((id << 1) & 2, id & 2);
    return float4(c * float2(2, -2) + float2(-1, 1), 0, 1);
}

struct Salida {
    uint4 entero : SV_Target0;
    float4 firmado : SV_Target1;
};

Salida PSTipos(float4 pos : SV_Position) {
    uint x = uint(pos.x), y = uint(pos.y);
    Salida s;
    s.entero = uint4(10 * x + 1, 20 * y + 2, 300, 7);
    s.firmado = float4(-0.25, 1.0, -1.0, 0.25);
    return s;
}
