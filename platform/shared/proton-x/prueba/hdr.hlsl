// hdr.hlsl -- los sombreadores de hdr.exe (N5.16 de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): los render targets de FLOAT.
// Cuentas exactas: cada valor cabe sin redondeo en su formato.

cbuffer Color : register(b0) {
    float4 color;
};
Texture2D<float4> hdr : register(t0);

// Un cuadro de pantalla completa sin bufer de vertices: las esquinas x de
// los vertices 1, 4 y 5 y las y de los 2, 3 y 5, en bits.
float4 VSCuadro(uint id : SV_VertexID) : SV_Position {
    float2 e = float2((0x32u >> id) & 1u, (0x2Cu >> id) & 1u);
    return float4(e.x * 2.0 - 1.0, 1.0 - e.y * 2.0, 0.5, 1.0);
}

// A y B: el color de las constantes de la raiz.
float4 PSColor() : SV_Target {
    return color;
}

// C: el render target de float LEIDO como textura, en un destino de 8 bits:
// lo que pasa de 1 solo sobrevive si se guardo en float.
float4 PSLee(float4 p : SV_Position) : SV_Target {
    return hdr.Load(int3(p.xy, 0)) * 0.25;
}
