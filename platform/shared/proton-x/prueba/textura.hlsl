// El sombreador de un cuadro con textura: la posicion tal cual y sus uv;
// el pixel, lo que la textura t0 dice en uv con el muestreador s0.
struct Salida
{
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD;
};

Texture2D imagen : register(t0);
SamplerState muestreo : register(s0);

Salida VSMain(float4 pos : POSITION, float4 uv : TEXCOORD)
{
    Salida s;
    s.pos = pos;
    s.uv = uv.xy;
    return s;
}

float4 PSMain(Salida e) : SV_TARGET
{
    return imagen.Sample(muestreo, e.uv);
}
