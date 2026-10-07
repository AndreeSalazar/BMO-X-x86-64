// uavorden.hlsl -- solo para el banco (A10, 06-10): un sombreador de pixeles
// que lee y escribe UAV y cuyo resultado depende del ORDEN de los pixeles
// (cada uno ve lo que dejaron los de antes). El juez es el interprete: lo
// TRADUCIDO (`nativo::compilar`, sus UAV por la llamada) tiene que dejar los
// mismos bits, pixel a pixel (`proton-x-casa/tests/nativo/uav.rs`).

RWByteAddressBuffer cuenta : register(u0);
RWStructuredBuffer<uint2> lista : register(u1);
RWTexture2D<float4> lienzo : register(u2);
RWBuffer<uint> tabla : register(u3);

float4 PSOrden(float4 p : SV_Position, float4 c : COLOR) : SV_Target {
    uint x = uint(p.x) & 7u;
    uint y = uint(p.y) & 7u;
    uint xy = x | (y << 3);
    uint antes;
    cuenta.InterlockedAdd(0, 1u, antes);
    lista[antes & 15u] = uint2(xy, asuint(c.x));
    float4 v = lienzo[uint2(x, y)] * 0.5 + c;
    lienzo[uint2(x, y)] = v;
    if (c.y < 0.0)
        discard;
    tabla[xy] = tabla[xy] + antes;
    for (uint i = 0; i < (asuint(c.z) & 3u); i++)
        cuenta.InterlockedMax(4, antes * 3u + i);
    uint w, h;
    lienzo.GetDimensions(w, h);
    return float4(v.xyz, float(w * h + cuenta.Load(4)));
}

// El de vertices: cada uno escribe en SU elemento y suma a un contador.
float4 VSOrden(float4 p : POSITION, uint id : SV_VertexID) : SV_Position {
    uint antes;
    cuenta.InterlockedAdd(8, asuint(p.x) & 255u, antes);
    tabla[id & 63u] = antes ^ asuint(p.y);
    return p * 2.0 + float(antes & 7u);
}
