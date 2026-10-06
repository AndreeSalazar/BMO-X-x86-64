// stencil.hlsl -- los sombreadores de stencil.exe (el STENCIL de D3D12,
// 05-10; la fila del stencil de la tabla 7.2 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md). Sin cuentas enteras ni saltos:
// el cuadro viene de un bufer de vertices y el color, de las constantes.

cbuffer Color : register(b0) {
    float4 color;
};

// La posicion tal cual, a z = 0.5.
float4 VSCuadro(float2 p : POSITION) : SV_Position {
    return float4(p, 0.5, 1.0);
}

// El color de las cuatro constantes de la raiz.
float4 PSColor() : SV_Target {
    return color;
}
