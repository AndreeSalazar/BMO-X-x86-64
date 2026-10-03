// N5.2 (03-10): cbuffers que no son b0 -- b1 (lee su SEGUNDA fila), b2 en
// space3, y b0 -- en un sombreador de pixeles.
cbuffer A : register(b1) { float4 a; float4 a2; };
cbuffer B : register(b2, space3) { float4 b; };
cbuffer K : register(b0) { float4 k; };
float4 pixel(float4 p : SV_Position, float2 uv : TEXCOORD0) : SV_Target {
  return a2 * 2 + b + k;
}
