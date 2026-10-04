// 03-10: lo que pidio la octava corrida de Cyberpunk: SampleCmpLevelZero
// (65, el PCF de las sombras), GatherRed (73) y firstbitlow (32), con sus
// hermanas firstbithigh y countbits.
Texture2D<float> sombra : register(t0);
Texture2D color : register(t1);
SamplerComparisonState cmp : register(s0);
SamplerState pt : register(s1);
float4 pixel(float4 p : SV_Position, float2 uv : TEXCOORD0, nointerpolation uint b : TEXCOORD1) : SV_Target {
  float s = sombra.SampleCmpLevelZero(cmp, uv, 0.5);
  float4 g = color.GatherRed(pt, uv);
  uint lo = firstbitlow(b);
  uint hi = firstbithigh(b);
  uint n = countbits(b);
  return float4(s, g.x + g.y * 10.0 + g.z * 100.0 + g.w * 1000.0, lo + hi * 100, n);
}
