// indice.hlsl -- N5.4 (05-10): dos arrays de texturas leidos con el registro
// CALCULADO (de un cbuffer): uno SIN medida (bindless, desde t5 de space1) y
// otro con medida (t1..t3 del espacio 0).
Texture2D m[] : register(t5, space1);
Texture2D f[3] : register(t1);
SamplerState s : register(s0);
cbuffer K : register(b0) { uint i; uint j; };
float4 pixel(float4 p : SV_Position, float2 uv : TEXCOORD0) : SV_Target {
  return m[i].Sample(s, uv) + f[j].Sample(s, uv);
}
