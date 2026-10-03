// N5.6 (03-10): la matematica que pedian los sombreadores de Cyberpunk
// (OperacionD3d 12, 13, 14, 21, 22, 23, 26, 27, 131 en el metal), una por
// componente, en tres salidas.
struct S { float4 a : SV_Target0; float4 b : SV_Target1; float4 c : SV_Target2; };
S pixel(float4 p : SV_Position, float4 x : TEXCOORD0, nointerpolation uint u : TEXCOORD1) {
  S s;
  s.a = float4(sin(x.x), cos(x.y), tan(x.z), exp2(x.w));
  s.b = float4(log2(x.x), frac(x.y), round(x.z), floor(x.w));
  s.c = float4(ceil(x.x), trunc(x.y), f16tof32(u), asfloat(f32tof16(x.z)));
  return s;
}
