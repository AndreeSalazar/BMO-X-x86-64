// capas1d.hlsl -- los sombreadores de capas1d.exe (06-10, A5): los UAV de
// un ARRAY de texturas de UNA dimension (RWTexture1DArray), escritos y
// leidos por computo. En ellos la CAPA es la segunda coordenada
// (u[uint2(x, capa)]) y GetDimensions da (ancho, capas). Todo exacto.

// u0: un array de 3 capas de 16 (RGBA32F), entero. u1: uno de 4 capas de 8
// con 2 mips (R32_UINT), por la vista de MipSlice = 1, FirstArraySlice = 1
// y ArraySize = 2 (de 4 de ancho). u2: uno de 2 capas de 4 (R32_UINT) para
// contar. u3: lo que se lee, de vuelta.
RWTexture1DArray<float4> fila : register(u0);
RWTexture1DArray<uint> medio : register(u1);
RWTexture1DArray<uint> cuenta : register(u2);
RWByteAddressBuffer sal : register(u3);

// A y C: cada hilo de 16 x 3, su texel; y suma 1 en la celda (x & 3, y & 1)
// (la capa 0 la tocan las filas 0 y 2: 8 por celda; la capa 1, 4).
[numthreads(16, 3, 1)]
void CSEscribe(uint3 id : SV_DispatchThreadID) {
    fila[id.xy] = float4(id.x, id.y, -(float)id.x, id.x + 16 * id.y);
    uint antes;
    InterlockedAdd(cuenta[uint2(id.x & 3, id.y & 1)], 1, antes);
}

// B: 8 x 4 hilos; la vista de `medio` tiene 4 de ancho y DOS capas: lo de
// x >= 4 o capa >= 2 cae fuera y no se escribe.
[numthreads(8, 4, 1)]
void CSMedio(uint3 id : SV_DispatchThreadID) {
    medio[id.xy] = 100 * id.y + id.x + 1;
}

// D, E y F: las medidas de las vistas y unas lecturas, a `sal`.
[numthreads(1, 1, 1)]
void CSLee() {
    uint w, n, w2, n2, w3, n3;
    fila.GetDimensions(w, n);
    medio.GetDimensions(w2, n2);
    cuenta.GetDimensions(w3, n3);
    sal.Store4(0, uint4(w, n, w2, n2));
    sal.Store2(16, uint2(w3, n3));
    sal.Store4(32, asuint(fila[uint2(5, 2)]));
    sal.Store(48, medio[uint2(3, 1)]);
    sal.Store(52, cuenta[uint2(2, 1)]);
    sal.Store(56, medio[uint2(1, 2)]);
    sal.Store(60, medio[uint2(9, 0)]);
    sal.Store4(64, asuint(fila[uint2(0, 7)]));
}
