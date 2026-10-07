// estructuras.hlsl -- 19 de la pila A de PLAN_LAS_TRES_GRANDES (07-10): un
// CS de Cyberpunk pidio "un array de algo que no es float ni entero
// (structs, vectores)". Un array COMPARTIDO de structs (un float3 y un uint)
// y uno de float2: cada hilo escribe el suyo, espera, y lee el de su espejo.
struct Particula {
    float3 pos;
    uint id;
};
RWStructuredBuffer<float4> salida : register(u0);
groupshared Particula datos[64];
groupshared float2 extra[64];

[numthreads(64, 1, 1)]
void CSMain(uint gi : SV_GroupIndex)
{
    Particula p;
    p.pos = float3(gi, gi * 0.5, -(float)gi);
    p.id = gi * 3 + 1;
    datos[gi] = p;
    extra[gi] = float2(gi + 0.25, 100.0 - gi);
    GroupMemoryBarrierWithGroupSync();
    Particula q = datos[63 - gi];
    float2 e = extra[63 - gi];
    salida[gi] = float4(q.pos.x + q.pos.y, q.pos.z, (float)q.id, e.x * e.y);
}
