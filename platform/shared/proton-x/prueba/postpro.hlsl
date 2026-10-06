// postpro.hlsl -- el CS de postpro.exe (06-10): lo que hace un posproceso
// de un juego en el computo. Cada hilo de 16 x 16: elige SU textura de un
// array SIN LIMITE por un indice calculado (el bindless:
// NonUniformResourceIndex), le suma lo que muestrea de la escena
// (SampleLevel, punto), lo escribe en un RWTexture2D y cuenta con un
// InterlockedAdd en un RWByteAddressBuffer; el primero, GetDimensions de su
// UAV.
Texture2D<float4> escena : register(t0);
Texture2D<float4> texs[] : register(t0, space1);
SamplerState punto : register(s0);
RWTexture2D<float4> salida : register(u0);
RWByteAddressBuffer cuenta : register(u1);
cbuffer K : register(b0) {
    uint cual;
    uint lado;
};

[numthreads(8, 8, 1)]
void CSPost(uint3 id : SV_DispatchThreadID) {
    uint i = (id.x + id.y + cual) % 3;
    float4 a = texs[NonUniformResourceIndex(i)].Load(int3(id.xy % 4, 0));
    float4 b = escena.SampleLevel(punto, (float2(id.xy) + 0.5) / lado, 0);
    salida[id.xy] = a + b;
    uint antes;
    cuenta.InterlockedAdd(4 * i, 1, antes);
    if (id.x == 0 && id.y == 0) {
        uint w, h;
        salida.GetDimensions(w, h);
        cuenta.Store(16, w * 1000 + h);
    }
}
