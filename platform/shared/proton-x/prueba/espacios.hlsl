Texture2D a : register(t3);
Texture2D b : register(t40, space1);
Texture2D c[4] : register(t7, space2);
SamplerState s0 : register(s20, space1);
SamplerState s1 : register(s2);
cbuffer K : register(b0) { float4 k; };
float4 pixel(float4 p : SV_Position, float2 uv : TEXCOORD0) : SV_Target {
  return a.Sample(s1, uv) + b.Sample(s0, uv) + c[2].Sample(s1, uv) + k;
}
