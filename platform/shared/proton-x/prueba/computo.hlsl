// computo.hlsl -- E2.3a de la ESCALERA (05-10): el COMPUTO de D3D12 (N5.5)
// en lo minimo que lo prueba entero. Cada hilo de un grupo de 64 copia su
// elemento a la memoria COMPARTIDA del grupo, espera a los demas (la
// barrera), y escribe el de su ESPEJO dentro del grupo: sin barrera, o sin
// memoria compartida de verdad, sale otra cosa. Lee un StructuredBuffer (SRV),
// escribe un RWStructuredBuffer (UAV), y usa los tres ids del hilo.
cbuffer K : register(b0) { uint n; float escala; };
StructuredBuffer<float4> entrada : register(t0);
RWStructuredBuffer<float4> salida : register(u0);
groupshared float4 compartida[64];

[numthreads(64, 1, 1)]
void CSMain(uint3 dt : SV_DispatchThreadID, uint3 g : SV_GroupID, uint gi : SV_GroupIndex)
{
    compartida[gi] = entrada[dt.x];
    GroupMemoryBarrierWithGroupSync();
    float4 v = compartida[63 - gi] * escala;
    if (dt.x < n)
        salida[dt.x] = float4(v.xyz, (float)(g.x * 1000 + gi));
}
