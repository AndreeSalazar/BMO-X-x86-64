// 02-10: las vistas que no son una 2D de nivel 0 (las que crea Cyberpunk).
Texture2DArray<float4> capas : register(t0);
TextureCube<float4> cubo : register(t1);
Texture3D<float4> volumen : register(t2);
Texture2D<float4> mips : register(t3);
Texture2D<uint4> enteros : register(t4);
SamplerState punto : register(s0);

struct E { float4 pos : SV_Position; float4 c : TEXCOORD0; nointerpolation int4 i : TEXCOORD1; };
struct S {
    float4 a : SV_Target0;
    float4 b : SV_Target1;
    float4 v : SV_Target2;
    float4 l : SV_Target3;
    float4 d : SV_Target4;
    float4 m : SV_Target5;
    float4 o : SV_Target6;
    float4 n : SV_Target7;
};

S pixel(E e) {
    S s;
    s.a = capas.Sample(punto, e.c.xyz);
    s.b = cubo.Sample(punto, e.c.xyz);
    s.v = volumen.Sample(punto, e.c.xyz);
    s.l = mips.SampleLevel(punto, e.c.xy, e.c.w);
    s.d = mips.Load(int3(e.i.xy, e.i.z));
    uint w, h, n;
    mips.GetDimensions(e.i.w, w, h, n);
    s.m = float4(w, h, n, 0);
    s.o = mips.Sample(punto, e.c.xy, int2(1, -1));
    s.n = float4(enteros.Load(int3(e.i.xy, 0)));
    return s;
}
