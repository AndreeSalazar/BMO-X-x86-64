// flotante1.hlsl -- los sombreadores de flotante1.exe (N5.16b de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): los render targets de UN
// float, los UAV de una textura de float y DepthClipEnable = FALSE.
// Cuentas exactas: cada valor cabe sin redondeo discutible en su formato.

cbuffer Color : register(b0) {
    float4 color;
};
Texture2D<float4> leida : register(t0);

// Un cuadro de pantalla completa sin bufer de vertices: las esquinas x de
// los vertices 1, 4 y 5 y las y de los 2, 3 y 5, en bits.
float2 esquina(uint id) {
    float2 e = float2((0x32u >> id) & 1u, (0x2Cu >> id) & 1u);
    return float2(e.x * 2.0 - 1.0, 1.0 - e.y * 2.0);
}

// A: el cuadro a media profundidad.
float4 VSCuadro(uint id : SV_VertexID) : SV_Position {
    return float4(esquina(id), 0.5, 1.0);
}

// C: el cuadro con la z en RAMPA: -0.5 a la izquierda, 1.5 a la derecha
// (w = 1): cruza el plano cercano (z = 0) y el lejano (z = 1).
float4 VSRampa(uint id : SV_VertexID) : SV_Position {
    float2 p = esquina(id);
    return float4(p, 0.5 + p.x, 1.0);
}

// A y C: el color de las constantes de la raiz.
float4 PSColor() : SV_Target {
    return color;
}

// A3: un render target de float LEIDO como textura (un R32F se lee
// (r, 0, 0, 1)), por el color.
float4 PSLee(float4 p : SV_Position) : SV_Target {
    return leida.Load(int3(p.xy, 0)) * color;
}

// B: un RWTexture2D de RGBA16F, leido y escrito en float. El 2^-16 no cabe
// en el half de 0.125 (su paso es 2^-13): se pierde AL ESCRIBIR.
RWTexture2D<float4> hdr : register(u0);
RWStructuredBuffer<float4> salida : register(u1);

[numthreads(4, 4, 1)]
void CSEscribe(uint2 xy : SV_DispatchThreadID) {
    hdr[xy] = hdr[xy] * float4(1.0, 1.0, 1.0, 60000.0) + float4(xy.x + 0.5, -(float)xy.y, 1.0 / 65536.0, 0.0);
}

// B: lo que quedo, leido por el sombreador (en float de 32: lo que se lee
// de vuelta es lo que se guardo en half).
[numthreads(8, 8, 1)]
void CSLee(uint2 xy : SV_DispatchThreadID) {
    salida[xy.y * 16 + xy.x] = hdr[xy];
}
