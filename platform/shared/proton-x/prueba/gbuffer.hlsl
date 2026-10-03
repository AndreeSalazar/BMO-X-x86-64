// N5.8 (03-10): un G-buffer como el de Cyberpunk, en chico: tres render
// targets (el 0, el 1 y el 3: el 2 se salta, como cuando una ranura queda
// sin vista), cada uno con lo suyo.
struct G {
  float4 albedo : SV_Target0;
  float4 sitio : SV_Target1;
  float4 normal : SV_Target3;
};
G pixel(float4 p : SV_Position, float2 uv : TEXCOORD) {
  G g;
  g.albedo = float4(1.0, 0.0, 0.0, 1.0);
  g.sitio = float4(p.x / 8.0, p.y / 8.0, 0.0, 1.0);
  g.normal = float4(0.0, 0.0, 1.0, 0.5);
  return g;
}
