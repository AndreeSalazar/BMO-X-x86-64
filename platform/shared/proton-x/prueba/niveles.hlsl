// E8g (09-10, DL17): las lecturas de textura que la 3060 ya emite, cada una
// por su forma (la especie de la PSV0) -- SampleLevel en 2D, en 3D y en un
// array de 2D, y Load en 2D y en un array --, sin desplazamientos, sumadas
// en una salida.
Texture2D<float4> plana : register(t0);
Texture3D<float4> volumen : register(t1);
Texture2DArray<float4> capas : register(t2);
SamplerState lineal : register(s0);
float4 pixel(float4 p : SV_Position, float4 uv : TEXCOORD0, nointerpolation int4 i : TEXCOORD1) : SV_Target {
  float4 a = plana.SampleLevel(lineal, uv.xy, uv.w);
  float4 b = volumen.SampleLevel(lineal, uv.xyz, uv.w);
  float4 c = capas.SampleLevel(lineal, uv.xyz, uv.w);
  float4 d = plana.Load(int3(i.xy, i.w));
  float4 e = capas.Load(int4(i.xyz, i.w));
  return a + b * 2 + c * 4 + d * 8 + e * 16;
}
