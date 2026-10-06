// derivadas.hlsl -- los sombreadores de derivadas.exe (D4.4 de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): las DERIVADAS de un cuadro de
// 2x2 y la MIP que eligen con ellas los muestreos. Cuentas exactas: cada
// valor cabe sin redondeo en su formato, y ningun LOD cae en un empate.

// Lo de cada cuadro dibujado, en las constantes de la raiz: su esquina y su
// lado en pixeles, el tramo de (u, v) que lo cubre, que lee y el lado del
// destino.
cbuffer Cuadro : register(b0) {
    float x0;
    float y0;
    float lado;
    float escala;
    uint modo;
    float ancho;
};

// t0: 64 x 64 con sus 7 mips, cada una de un color solido. t1: la misma
// con MostDetailedMip = 2. t2: la misma con ResourceMinLODClamp = 4.
Texture2D t0 : register(t0);
Texture2D t1 : register(t1);
Texture2D t2 : register(t2);
// s0 MIN_MAG_MIP_POINT y s1 MIN_MAG_POINT_MIP_LINEAR (estaticos); s2 en un
// monton (CreateSampler): punto con MaxLOD = 1.
SamplerState punto : register(s0);
SamplerState trilineal : register(s1);
SamplerState tope : register(s2);

struct V {
    float4 p : SV_Position;
    float2 uv : UV;
};

// Un cuadro sin bufer de vertices: las esquinas x de los vertices 1, 4 y 5
// y las y de los 2, 3 y 5, en bits; (u, v) de 0 a `escala`.
V VSCuadro(uint id : SV_VertexID) {
    float2 e = float2((0x32u >> id) & 1u, (0x2Cu >> id) & 1u);
    float2 px = float2(x0, y0) + e * lado;
    V v;
    v.p = float4(px.x / ancho * 2.0 - 1.0, 1.0 - px.y / ancho * 2.0, 0.5, 1.0);
    v.uv = e * escala;
    return v;
}

// A: las derivadas de f = x * y (x e y, el centro del pixel: n + 0,5). Las
// finas, de su fila y su columna: ddx = y, ddy = x. Las gruesas, una para
// el cuadro: la de una de sus dos filas (o columnas).
float4 PSDerivadas(float4 p : SV_Position) : SV_Target {
    float f = p.x * p.y;
    return float4(ddx_fine(f), ddy_fine(f), ddx_coarse(f), ddy_coarse(f));
}

// B, C, D y E: lo que dice `modo`.
float4 PSMips(V v) : SV_Target {
    if (modo == 0)
        return t0.Sample(punto, v.uv);
    if (modo == 1)
        return t0.SampleBias(trilineal, v.uv, 0.5);
    if (modo == 2)
        return t0.SampleLevel(trilineal, v.uv, 2.5);
    if (modo == 3)
        return t0.SampleGrad(punto, v.uv, float2(0.125, 0.0), float2(0.0, 0.125));
    if (modo == 4)
        return float4(t0.CalculateLevelOfDetail(trilineal, v.uv) * 32.0 / 255.0, (t0.CalculateLevelOfDetailUnclamped(trilineal, v.uv) + 4.0) * 16.0 / 255.0, 0.0, 1.0);
    if (modo == 5)
        return t1.Sample(punto, v.uv);
    if (modo == 6)
        return t2.Sample(punto, v.uv);
    return t0.Sample(tope, v.uv);
}
