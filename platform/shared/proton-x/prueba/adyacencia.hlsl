// adyacencia.hlsl -- los sombreadores de adyacencia.exe (06-10, A6): las
// topologias con ADYACENCIA (`*_ADJ`). Un GS `triangleadj` o `lineadj`
// apunta, por cada primitiva, los vertices que le llegan y en que orden; sin
// GS, solo se pintan los triangulos (los vertices de al lado no cuentan).
// Todo entero.

RWByteAddressBuffer sal : register(u0);
// Donde apunta este dibujo (bytes), y que tabla de posiciones lee el VS.
cbuffer caso : register(b0) {
    uint base;
    uint tabla;
};

struct V {
    float4 pos : SV_Position;
    uint id : ID;
};

// El vertice: su SV_VertexID (lo que apunta el GS) y, sin GS, una posicion
// de la tabla 1 (un triangulo que tapa la pantalla con los vertices 0, 2 y
// 4; los impares, todos en la esquina: con ellos no sale ningun triangulo
// con area) o de la 2 (un cuadro con los 0, 2, 4 y 6).
V VSId(uint id : SV_VertexID) {
    static const float2 uno[6] = {float2(-1, -1), float2(-1, -1), float2(-1, 3), float2(-1, -1), float2(3, -1), float2(-1, -1)};
    static const float2 dos[8] = {float2(-1, -1), float2(-1, -1), float2(-1, 1), float2(-1, -1), float2(1, -1), float2(-1, -1), float2(1, 1), float2(-1, -1)};
    V v;
    float2 p = tabla == 1 ? uno[min(id, 5u)] : dos[min(id, 7u)];
    v.pos = float4(p, 0.5, 1.0);
    v.id = id;
    return v;
}

// Cada primitiva apunta sus 6 ids (32 bytes por primitiva) y suma 1 en la
// cuenta del dibujo (+240). Emite UN vertice: un triangulo a medias, que no
// pinta nada.
[maxvertexcount(3)]
void GSTri(triangleadj V p[6], uint prim : SV_PrimitiveID, inout TriangleStream<V> s) {
    uint o = base + 32 * prim;
    sal.Store4(o, uint4(p[0].id, p[1].id, p[2].id, p[3].id));
    sal.Store2(o + 16, uint2(p[4].id, p[5].id));
    sal.InterlockedAdd(base + 240, 1);
    s.Append(p[0]);
}

// Lo mismo con 4 (16 bytes por primitiva).
[maxvertexcount(3)]
void GSLinea(lineadj V p[4], uint prim : SV_PrimitiveID, inout TriangleStream<V> s) {
    sal.Store4(base + 16 * prim, uint4(p[0].id, p[1].id, p[2].id, p[3].id));
    sal.InterlockedAdd(base + 240, 1);
    s.Append(p[0]);
}

float4 PSBlanco() : SV_Target {
    return float4(1.0, 1.0, 1.0, 1.0);
}
