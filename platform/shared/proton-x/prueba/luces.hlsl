// 03-10: un ARRAY de un cbuffer leido con indice calculado (las luces, los
// huesos): `CBufferLoadLegacy` con la fila que no es una constante.
cbuffer Luces : register(b0) {
  float4 color[8];
  float4 extra;
};
float4 pixel(float4 p : SV_Position, nointerpolation uint i : TEXCOORD0) : SV_Target {
  return color[i & 7] + extra;
}
