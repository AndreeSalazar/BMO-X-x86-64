// 03-10: las olas y las derivadas que pedian los sombreadores de Cyberpunk
// (OperacionD3d 83, 84 y 118 en la quinta corrida), con un pixel por ola.
float4 pixel(float4 p : SV_Position, float4 x : TEXCOORD0, nointerpolation uint u : TEXCOORD1) : SV_Target {
  float a = WaveReadLaneFirst(x.x);
  float b = WaveActiveSum(x.y);
  uint n = WaveGetLaneCount() + WavePrefixSum(u) + WaveGetLaneIndex();
  uint k = WaveActiveCountBits(x.y > 0.0);
  float d = ddx(x.z) + ddy(x.w) + fwidth(x.x);
  bool f = WaveIsFirstLane() && WaveActiveAllTrue(x.x > 0.0) && WaveActiveAnyTrue(x.z > 1.0);
  return float4(a, b, (float)(n * 10 + k) + d, f ? 1.0 : 0.0);
}
