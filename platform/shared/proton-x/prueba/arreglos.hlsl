// N5.10 (03-10): los arrays que pedian los sombreadores de Cyberpunk
// (`Instruccion(19)` alloca y `(43)` getelementptr): uno local con indice
// calculado, uno de dos dimensiones escrito en un bucle, y dos tablas
// globales constantes (float e int).
static const float pesos[4] = {0.1, 0.2, 0.3, 0.4};
static const int saltos[3] = {5, -2, 7};
float4 pixel(float4 p : SV_Position, nointerpolation uint i : TEXCOORD0, float4 x : TEXCOORD1) : SV_Target {
  float a[4] = { x.x, x.y, x.z, x.w };
  a[i & 3] += 1.0;
  float m[2][3];
  [loop] for (uint k = 0; k < 6; k++)
    m[k / 3][k % 3] = k * 10.0;
  float s = 0;
  [loop] for (uint j = 0; j < 4; j++)
    s += a[j] * pesos[j];
  return float4(a[(i + 1) & 3], s, m[i & 1][2] + saltos[i % 3], a[0]);
}
