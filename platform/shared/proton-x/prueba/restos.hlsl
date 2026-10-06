// restos.hlsl -- los sombreadores de restos.exe (05-10, lo que QUEDABA de
// D3D12 en N5.3d, N5.12b y N5.16b de docs/plan/PLAN_LAS_TRES_GRANDES.md):
// render targets de ENTEROS, dibujos SOLO con UAV, UAV en un sombreador de
// GEOMETRIA, el plano de STENCIL leido de vuelta y SV_StencilRef. Todo
// entero: nada que redondear con una GPU de verdad.

// Ocho constantes de la raiz: un valor y un paso por canal.
cbuffer valores : register(b0) {
    uint4 v;
    uint4 d;
};

Texture2D<uint> leido : register(t0);
Texture2D<uint2> plano : register(t1);
RWByteAddressBuffer cuenta : register(u0);
RWTexture2D<uint> marcas : register(u1);
RWBuffer<uint> lista : register(u2);

// Un triangulo que cubre la pantalla entera, sin bufer de vertices.
float4 VSPantalla(uint id : SV_VertexID) : SV_Position {
    float2 e = float2((id << 1) & 2u, id & 2u);
    return float4(e.x * 2.0 - 1.0, 1.0 - e.y * 2.0, 0.5, 1.0);
}

// A: lo que sale a un render target de enteros: v + d * (x + 16 y).
uint4 PSUint(float4 p : SV_Position) : SV_Target {
    uint2 xy = uint2(p.xy);
    return v + d * (xy.x + 16u * xy.y);
}

int4 PSSint(float4 p : SV_Position) : SV_Target {
    uint2 xy = uint2(p.xy);
    return int4(v + d * (xy.x + 16u * xy.y));
}

// A4: dos render targets de enteros a la vez (el 0 con mascara de escritura).
struct Dos {
    uint4 a : SV_Target0;
    uint2 b : SV_Target1;
};

Dos PSDos() {
    Dos o;
    o.a = v;
    o.b = d.xy;
    return o;
}

// A5: un R32_UINT leido como textura (Load), mas 1.
uint PSLee(float4 p : SV_Position) : SV_Target {
    return leido.Load(int3(int2(p.xy), 0)) + 1u;
}

// D2: el plano de stencil por un SRV X24_TYPELESS_G8_UINT: el stencil en G.
uint PSStencil(float4 p : SV_Position) : SV_Target {
    return plano.Load(int3(int2(p.xy), 0)).g;
}

// B: SIN render target ni profundidad: cada pixel suma 1 y marca su texel.
void PSSoloUav(float4 p : SV_Position) {
    uint2 xy = uint2(p.xy);
    cuenta.InterlockedAdd(0, 1);
    marcas[xy] = 1000u + xy.x + 16u * xy.y;
}

// C: un GS que no pinta nada (emite UN vertice: una tira sin triangulos) y
// escribe UAV: suma 1 por punto en la segunda palabra y pone 200 + el numero
// de su vertice en su elemento de la lista.
struct Punto {
    float4 pos : SV_Position;
    uint id : NUMERO;
};

Punto VSPunto(uint id : SV_VertexID) {
    Punto o;
    o.pos = float4(0.0, 0.0, 0.5, 1.0);
    o.id = id;
    return o;
}

struct Nada {
    float4 pos : SV_Position;
};

[maxvertexcount(3)]
void GSMarca(point Punto p[1], inout TriangleStream<Nada> s) {
    cuenta.InterlockedAdd(4, 1);
    lista[p[0].id] = 200u + p[0].id;
    Nada o;
    o.pos = p[0].pos;
    s.Append(o);
}

// E: el de pixeles da la referencia de stencil: v.y + v.z * x; el color, v.x.
struct ConRef {
    uint c : SV_Target;
    uint ref : SV_StencilRef;
};

ConRef PSRef(float4 p : SV_Position) {
    ConRef o;
    o.c = v.x;
    o.ref = v.y + v.z * uint(p.x);
    return o;
}
