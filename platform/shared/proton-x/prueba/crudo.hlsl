// crudo.hlsl -- 13 de la pila A de PLAN_LAS_TRES_GRANDES (07-10):
// RawBufferLoad y RawBufferStore (op 139 y 140), lo que dxc da desde SM 6.2
// para ByteAddressBuffer y StructuredBuffer. Un CS de Cyberpunk lo pidio en
// el metal (06-10: `OperacionD3d(139)`). Cada hilo lee bytes sueltos (Load
// y Load4 de un ByteAddressBuffer), una fila de un StructuredBuffer, y
// escribe bytes (Store y Store2) y una fila de un RWStructuredBuffer.
cbuffer K : register(b0) { uint n; };
ByteAddressBuffer bytes : register(t0);
StructuredBuffer<float4> filas : register(t1);
RWByteAddressBuffer salida : register(u0);
RWStructuredBuffer<uint2> pares : register(u1);

[numthreads(32, 1, 1)]
void CSMain(uint3 dt : SV_DispatchThreadID)
{
    uint i = dt.x;
    if (i >= n)
        return;
    uint a = bytes.Load(i * 4);
    uint4 b = bytes.Load4(128 + i * 16);
    float4 f = filas[i];
    salida.Store(i * 4, a * 3 + 1);
    salida.Store2(256 + i * 8, uint2(b.x ^ b.w, asuint(f.y * 2.0)));
    pares[i] = uint2(b.y + b.z, asuint(f.x + f.w));
}
