// cubo.hlsl -- los dos sombreadores del cubo de P3b, en HLSL, con LAS MISMAS
// cuentas que el cubo de X1 (bmo-cubo, `constantes` e `iluminar`): lo que
// D3D12 dibujo en la 3060 y dejo sus huellas en bmo_cubo::referencia.
//
//    vertice   posicion de recorte = mul(wvp, float4(pos, 1)); la normal al
//              mundo con la parte 3x3 de `world`; el color, tal cual
//    pixel     rgb * (ambiente + (1 - ambiente) * saturate(dot(n, L)))
//
// Las matrices van por columnas y se usan con mul(M, v), como en bmo-cubo.
// Se compilan con dxc (el oficial, SM 6.0) a DXIL: prueba/HACER.txt.

cbuffer Constantes : register(b0) {
    float4x4 wvp;
    float4x4 world;
    float4 luz; // xyz: hacia la luz, ya normalizada; w: ambiente
};

struct Entrada {
    float3 pos : POSITION;
    float3 normal : NORMAL;
    float4 color : COLOR;
};

struct Salida {
    float4 pos : SV_Position;
    float3 normal : NORMAL;
    float4 color : COLOR;
};

Salida vertice(Entrada e) {
    Salida s;
    s.pos = mul(wvp, float4(e.pos, 1.0));
    s.normal = mul((float3x3)world, e.normal);
    s.color = e.color;
    return s;
}

float4 pixel(Salida s) : SV_Target {
    float d = saturate(dot(normalize(s.normal), luz.xyz));
    float k = luz.w + (1.0 - luz.w) * d;
    return float4(s.color.rgb * k, s.color.a);
}
