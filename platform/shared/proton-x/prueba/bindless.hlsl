// bindless.hlsl -- 15 de la pila A de PLAN_LAS_TRES_GRANDES (07-10):
// "createHandle con un registro CALCULADO de un array de BUFERES", lo que
// un CS de Cyberpunk pidio en el metal (06-10). Dos arrays de buferes en el
// espacio 1, elegidos por hilo: una fila de un StructuredBuffer, sus
// medidas (GetDimensions), y una palabra de un ByteAddressBuffer.
StructuredBuffer<uint4> tablas[4] : register(t0, space1);
ByteAddressBuffer crudos[2] : register(t8, space1);
RWStructuredBuffer<uint4> salida : register(u0);

[numthreads(16, 1, 1)]
void CSMain(uint gi : SV_GroupIndex)
{
    uint k = gi & 3;
    uint4 v = tablas[NonUniformResourceIndex(k)][gi];
    uint n, paso;
    tablas[NonUniformResourceIndex(k)].GetDimensions(n, paso);
    uint c = crudos[NonUniformResourceIndex(gi & 1)].Load(gi * 4);
    salida[gi] = uint4(v.x + v.w, n, paso, c);
}
