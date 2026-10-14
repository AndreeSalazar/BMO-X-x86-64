// 14-10 (el back buffer de Cyberpunk): un pixel que LEE su textura con
// `Load` -- sin muestreador --, como el ultimo pase de un juego. Los
// vertices, los de `textura.hlsl` (`textura_vs.dxil`).
struct Salida
{
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD;
};

Texture2D imagen : register(t0);

float4 PSMain(Salida e) : SV_TARGET
{
    return imagen.Load(int3(e.uv * 8.0, 0));
}
