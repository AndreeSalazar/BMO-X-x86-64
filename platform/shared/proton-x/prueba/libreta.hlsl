// libreta.hlsl -- los sombreadores de libreta.exe (06-10, 9d del contador de
// DX12): un dibujo NORMAL y uno RARO -- su de pixeles saca +infinito, NaN y
// -infinito a proposito --, los dos a un RGBA8_UNORM. D3D dice como se
// guarda lo raro (+inf a 1, NaN a 0, -inf a 0): la imagen es la misma en
// Windows y en BMO-X. En el metal de BMO-X, ademas, la LIBRETA de la 3060
// lo apunta (el termometro del cuerpo, `proton-x-sm86::libreta`).
//
// `grande` es k.x * k.x: con k.x = 1 da 1; con k.x = 1e30 da +inf, y por
// k.y = 0 da NaN (el cbuffer no se pliega al compilar; `grande - grande`
// SI: dxc lo dejaba en 0). Sin Div: la 3060 la rechaza (va por la CPU), y el
// dibujo raro tiene que ir a ella.

cbuffer K : register(b0) {
    float4 k;
};

struct V {
    float4 pos : SV_Position;
    float4 color : COLOR;
};

V VSPasa(float3 pos : POSITION, float4 color : COLOR) {
    V v;
    v.pos = float4(pos, 1.0);
    v.color = color;
    return v;
}

float4 PSRaro(V v) : SV_Target {
    float grande = k.x * k.x;
    return float4(v.color.x * grande, v.color.y * (grande * k.y), -v.color.z * grande, v.color.w);
}
