// N5.3 (03-10): los tres BUFERES que lee un sombreador -- con tipo,
// estructurado (dentro de su elemento) y crudo (por bytes) -- y el
// GetDimensions de uno estructurado.
struct S { float a; float4 b; };
Buffer<float4> tipado : register(t0);
StructuredBuffer<S> estructurado : register(t1);
ByteAddressBuffer crudo : register(t2, space1);
float4 pixel(float4 p : SV_Position, nointerpolation uint4 i : TEXCOORD0) : SV_Target {
  uint n, paso;
  estructurado.GetDimensions(n, paso);
  return tipado.Load(i.x) + estructurado[i.y].b + asfloat(crudo.Load4(i.z)) + n;
}
