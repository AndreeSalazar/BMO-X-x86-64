// 03-10: el de pixeles que escribe su PROFUNDIDAD (SV_Depth), como los de
// Cyberpunk que pedia el metal: la Z sale del sombreador, no del triangulo.
struct S {
  float4 color : SV_Target;
  float z : SV_Depth;
};
S pixel(float4 p : SV_Position) {
  S s;
  s.color = float4(1.0, 1.0, 1.0, 1.0);
  s.z = p.x / 8.0;
  return s;
}
