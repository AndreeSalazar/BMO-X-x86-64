// salidas.hlsl -- 16 y 21 de la pila A de PLAN_LAS_TRES_GRANDES (07-10):
// "un bucle con mas de una salida", lo que pidieron un CS y un sombreador
// de pixeles de Cyberpunk en el metal (06-10). Un bucle que sale por TRES
// sitios: su condicion, un `break` (la suma se pasa) y un `return` (lo
// encontro). Cada hilo busca en la tabla con su propio xor; lo que escribe
// dice por donde salio, en que vuelta y con que suma.
cbuffer K : register(b0) { uint n; uint buscado; uint tope; };
StructuredBuffer<uint> datos : register(t0);
RWStructuredBuffer<uint4> salida : register(u0);

[numthreads(64, 1, 1)]
void CSMain(uint gi : SV_GroupIndex)
{
    uint suma = 0;
    uint k;
    [loop]
    for (k = 0; k < n; k++) {
        uint v = datos[k] ^ gi;
        if (v == buscado) {
            salida[gi] = uint4(1, k, suma, v);
            return;
        }
        suma += v;
        if (suma > tope)
            break;
    }
    salida[gi] = uint4(suma > tope ? 2 : 3, k, suma, 0);
}

// El de pixeles (el 21): lo mismo, con la x del pixel como xor; la vuelta y
// la suma salen como color.
float4 PSMain(float4 pos : SV_Position) : SV_Target
{
    uint gi = (uint)pos.x;
    uint suma = 0;
    uint k;
    [loop]
    for (k = 0; k < n; k++) {
        uint v = datos[k] ^ gi;
        if (v == buscado)
            return float4(1, k, suma, v);
        suma += v;
        if (suma > tope)
            break;
    }
    return float4(suma > tope ? 2 : 3, k, suma, 0);
}
