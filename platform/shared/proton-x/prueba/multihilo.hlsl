// multihilo.hlsl -- los sombreadores de multihilo.exe (E2.1 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md, 05-10): listas grabadas desde
// varios hilos y varias colas con vallas. Cuentas exactas: colores que caben
// en 8 bits sin redondeo que discutir, profundidades que son potencias de 2
// y enteros.

cbuffer Raiz : register(b0) {
    float4 color;
    float z;
};
RWByteAddressBuffer datos : register(u0);

// Un cuadro de pantalla completa sin bufer de vertices (el de hdr.hlsl), a
// la profundidad de la raiz: la tijera dice que parte se pinta.
float4 VSCuadro(uint id : SV_VertexID) : SV_Position {
    float2 e = float2((0x32u >> id) & 1u, (0x2Cu >> id) & 1u);
    return float4(e.x * 2.0 - 1.0, 1.0 - e.y * 2.0, z, 1.0);
}

// El color de la raiz.
float4 PSColor() : SV_Target {
    return color;
}

// Leer, doblar y sumar el indice: correrlo antes de que llegue el dato da
// otro numero, y correrlo dos veces, 4x + 3i.
[numthreads(64, 1, 1)]
void CSDoble(uint i : SV_DispatchThreadID) {
    datos.Store(i * 4, datos.Load(i * 4) * 2 + i);
}
